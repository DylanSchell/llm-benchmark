use serde::{Deserialize, Serialize};

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
    #[serde(rename = "uuid")]
    pub uuid: Option<String>,
    #[serde(rename = "parentUuid")]
    pub parent_uuid: Option<String>,
    #[serde(rename = "isSidechain")]
    pub is_sidechain: Option<bool>,
    #[serde(rename = "userType")]
    pub user_type: Option<String>,
    pub cwd: Option<String>,
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    pub version: Option<String>,
    #[serde(rename = "gitBranch")]
    pub git_branch: Option<String>,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueOperationEntry {
    #[serde(flatten)]
    pub base: BaseLogEntry,
    pub operation: Option<String>,
    pub content: Option<serde_json::Value>,
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
    #[serde(default)]
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
    // Direct token fields (some JSON formats put them here instead of in usage)
    #[serde(rename = "input_tokens", default)]
    pub input_tokens_direct: Option<u64>,
    #[serde(rename = "output_tokens", default)]
    pub output_tokens_direct: Option<u64>,
    #[serde(rename = "cache_read_input_tokens", default)]
    pub cache_read_direct: Option<u64>,
    #[serde(rename = "cache_creation_input_tokens", default)]
    pub cache_write_direct: Option<u64>,
    pub container: Option<Container>,
    // Content can be either a string (user prompts) or Vec<Content> (assistant responses)
    pub content: Option<MessageContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextManagement {
    pub active_form: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    #[serde(rename = "cache_read_input_tokens", default)]
    pub cache_read_input_tokens: u64,
    #[serde(rename = "cache_creation_input_tokens", default)]
    pub cache_creation_input_tokens: u64,
    #[serde(rename = "input_tokens", default)]
    pub input_tokens: u64,
    #[serde(rename = "output_tokens", default)]
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
    #[serde(rename = "ephemeral_1h_input_tokens", default)]
    pub ephemeral_1h_input_tokens: u64,
    #[serde(rename = "ephemeral_5m_input_tokens", default)]
    pub ephemeral_5m_input_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerToolUse {
    #[serde(default)]
    pub web_search_requests: Option<u64>,
    #[serde(default)]
    pub web_fetch_requests: Option<u64>,
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceTier(pub String);

impl<'de> serde::Deserialize<'de> for ServiceTier {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::String(s) => Ok(ServiceTier(s)),
            serde_json::Value::Object(map) => {
                if let Some(t) = map.get("type").and_then(|v| v.as_str()) {
                    Ok(ServiceTier(t.to_string()))
                } else {
                    Ok(ServiceTier(String::new()))
                }
            }
            _ => Ok(ServiceTier(String::new())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    pub id: Option<String>,
}

/// Content can be either a plain string (user prompts) or structured content blocks (assistant responses)
#[derive(Debug, Clone, Serialize)]
pub enum MessageContent {
    Text(String),
    Structured(Vec<Content>),
}

impl<'de> serde::Deserialize<'de> for MessageContent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::String(s) => Ok(MessageContent::Text(s)),
            serde_json::Value::Array(arr) => {
                let items: Result<Vec<Content>, _> = arr.into_iter()
                    .map(|v| serde_json::from_value(v))
                    .collect();
                items.map_err(serde::de::Error::custom).map(MessageContent::Structured)
            }
            _ => Ok(MessageContent::Text(value.to_string())),
        }
    }
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

// =============================================================================
// Pi Agent Log Entry Models
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PiLogEntry {
    #[serde(rename = "session")]
    Session(PiSession),
    #[serde(rename = "model_change")]
    ModelChange(PiModelChange),
    #[serde(rename = "thinking_level_change")]
    ThinkingLevelChange(PiThinkingLevelChange),
    #[serde(rename = "message")]
    Message(PiMessage),
    #[serde(rename = "tool_execution_start")]
    ToolExecutionStart(PiToolExecutionStart),
    #[serde(rename = "tool_execution_update")]
    ToolExecutionUpdate(PiToolExecutionUpdate),
    #[serde(rename = "tool_execution_end")]
    ToolExecutionEnd(PiToolExecutionEnd),
    #[serde(rename = "auto_compaction_start")]
    AutoCompactionStart(PiAutoCompactionStart),
    #[serde(rename = "auto_compaction_end")]
    AutoCompactionEnd(PiAutoCompactionEnd),
    #[serde(rename = "auto_retry_start")]
    AutoRetryStart(PiAutoRetryStart),
    #[serde(rename = "auto_retry_end")]
    AutoRetryEnd(PiAutoRetryEnd),
    #[serde(rename = "compaction")]
    Compaction(PiCompaction),
    #[serde(rename = "branch_summary")]
    BranchSummary(PiBranchSummary),
    #[serde(rename = "custom_message")]
    CustomMessage(PiCustomMessage),
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Serialize)]
pub struct PiBaseEntry {
    pub id: Option<String>,
    pub parent_id: Option<String>,
    #[serde(rename = "parentId")]
    pub parent_id_alt: Option<String>,
    #[serde(deserialize_with = "deserialize_timestamp")]
    pub timestamp: Option<String>,
}

fn deserialize_timestamp<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::String(s) => Ok(Some(s)),
        serde_json::Value::Number(n) => Ok(n.to_string().into()),
        _ => Ok(None),
    }
}

impl<'de> serde::Deserialize<'de> for PiBaseEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct Helper {
            id: Option<String>,
            #[serde(rename = "parentId")]
            parent_id_alt: Option<String>,
            #[serde(deserialize_with = "deserialize_timestamp")]
            timestamp: Option<String>,
        }
        let h = Helper::deserialize(deserializer)?;
        Ok(PiBaseEntry {
            id: h.id.clone(),
            parent_id: h.parent_id_alt.clone().or(h.id),
            parent_id_alt: h.parent_id_alt,
            timestamp: h.timestamp,
        })
    }
}

impl PiBaseEntry {
    pub fn parent_id(&self) -> Option<&str> {
        self.parent_id.as_deref().or(self.parent_id_alt.as_deref())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiSession {
    #[serde(flatten)]
    pub base: PiBaseEntry,
    pub version: Option<u32>,
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiModelChange {
    #[serde(flatten)]
    pub base: PiBaseEntry,
    pub provider: Option<String>,
    #[serde(rename = "modelId")]
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiThinkingLevelChange {
    #[serde(flatten)]
    pub base: PiBaseEntry,
    #[serde(rename = "thinkingLevel")]
    pub thinking_level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiMessage {
    #[serde(flatten)]
    pub base: PiBaseEntry,
    pub message: Option<PiMessageData>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PiMessageData {
    pub api: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub role: Option<String>,
    pub content: Option<Vec<PiContentItem>>,
    #[serde(deserialize_with = "deserialize_timestamp")]
    pub timestamp: Option<String>,
    pub usage: Option<PiUsage>,
    #[serde(rename = "stopReason")]
    pub stop_reason: Option<String>,
    #[serde(rename = "toolCallId")]
    pub tool_call_id: Option<String>,
    #[serde(rename = "toolName")]
    pub tool_name: Option<String>,
    #[serde(rename = "isError")]
    pub is_error: Option<bool>,
    #[serde(rename = "responseId")]
    pub response_id: Option<String>,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
}

impl<'de> serde::Deserialize<'de> for PiMessageData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct Helper {
            api: Option<String>,
            provider: Option<String>,
            model: Option<String>,
            role: Option<String>,
            content: Option<Vec<PiContentItem>>,
            #[serde(deserialize_with = "deserialize_timestamp")]
            timestamp: Option<String>,
            usage: Option<PiUsage>,
            #[serde(rename = "stopReason")]
            stop_reason: Option<String>,
            #[serde(rename = "toolCallId")]
            tool_call_id: Option<String>,
            #[serde(rename = "toolName")]
            tool_name: Option<String>,
            #[serde(rename = "isError")]
            is_error: Option<bool>,
            #[serde(rename = "responseId")]
            response_id: Option<String>,
            #[serde(rename = "errorMessage")]
            error_message: Option<String>,
        }
        let h = Helper::deserialize(deserializer)?;
        Ok(PiMessageData {
            api: h.api,
            provider: h.provider,
            model: h.model,
            role: h.role,
            content: h.content,
            timestamp: h.timestamp,
            usage: h.usage,
            stop_reason: h.stop_reason,
            tool_call_id: h.tool_call_id,
            tool_name: h.tool_name,
            is_error: h.is_error,
            response_id: h.response_id,
            error_message: h.error_message,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PiContentItem {
    #[serde(rename = "toolCall")]
    ToolCall(PiToolCall),
    #[serde(rename = "text")]
    Text(PiTextContent),
    #[serde(rename = "thinking")]
    Thinking(PiThinkingContent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiToolCall {
    pub id: Option<String>,
    pub name: Option<String>,
    pub arguments: Option<serde_json::Value>,
    #[serde(rename = "partialArgs")]
    pub partial_args: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiTextContent {
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiThinkingContent {
    pub thinking: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiUsage {
    pub input: Option<u64>,
    pub output: Option<u64>,
    #[serde(rename = "cacheRead")]
    pub cache_read: Option<u64>,
    #[serde(rename = "cacheWrite")]
    pub cache_write: Option<u64>,
    #[serde(rename = "totalTokens")]
    pub total_tokens: Option<u64>,
    pub cost: Option<PiCost>,
}

impl PiUsage {
    pub fn total_input(&self) -> u64 {
        self.input.unwrap_or(0)
            + self.cache_read.unwrap_or(0)
            + self.cache_write.unwrap_or(0)
    }

    pub fn total_output(&self) -> u64 {
        self.output.unwrap_or(0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiCost {
    pub total: Option<f64>,
    pub input: Option<f64>,
    pub output: Option<f64>,
    pub cache_read: Option<f64>,
    pub cache_write: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiToolExecutionStart {
    #[serde(flatten)]
    pub base: PiBaseEntry,
    #[serde(rename = "toolName")]
    pub tool_name: Option<String>,
    pub args: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiToolExecutionUpdate {
    #[serde(flatten)]
    pub base: PiBaseEntry,
    #[serde(rename = "toolName")]
    pub tool_name: Option<String>,
    #[serde(rename = "partialResult")]
    pub partial_result: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiToolExecutionEnd {
    #[serde(flatten)]
    pub base: PiBaseEntry,
    #[serde(rename = "toolName")]
    pub tool_name: Option<String>,
    #[serde(rename = "isError")]
    pub is_error: Option<bool>,
    pub result: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiAutoCompactionStart {
    #[serde(flatten)]
    pub base: PiBaseEntry,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiAutoCompactionEnd {
    #[serde(flatten)]
    pub base: PiBaseEntry,
    pub aborted: Option<bool>,
    #[serde(rename = "willRetry")]
    pub will_retry: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiAutoRetryStart {
    #[serde(flatten)]
    pub base: PiBaseEntry,
    pub attempt: Option<u32>,
    #[serde(rename = "maxAttempts")]
    pub max_attempts: Option<u32>,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiAutoRetryEnd {
    #[serde(flatten)]
    pub base: PiBaseEntry,
    pub success: Option<bool>,
    pub attempt: Option<u32>,
    #[serde(rename = "finalError")]
    pub final_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiCompaction {
    #[serde(flatten)]
    pub base: PiBaseEntry,
    pub summary: Option<String>,
    #[serde(rename = "tokensBefore")]
    pub tokens_before: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiBranchSummary {
    #[serde(flatten)]
    pub base: PiBaseEntry,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiCustomMessage {
    #[serde(flatten)]
    pub base: PiBaseEntry,
    #[serde(rename = "customType")]
    pub custom_type: Option<String>,
    pub content: Option<String>,
    pub display: Option<bool>,
}
