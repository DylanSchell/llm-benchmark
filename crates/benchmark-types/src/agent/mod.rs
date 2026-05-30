use serde::{Deserialize, Serialize, Serializer};
use std::path::Path;

fn serialize_duration_seconds<S>(duration_ms: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_f64(*duration_ms as f64 / 1000.0)
}

#[async_trait::async_trait]
pub trait Agent: Send + Sync {
    async fn run_exercise(
        &self,
        exercise: &crate::exercise::Exercise,
        host_exercise_dir: &Path,
        model: &str,
        thinking_level: Option<&str>,
        results_dir: &Path,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>>;

    /// Returns the agent's name (e.g., "reference", "claude", "pi").
    fn get_name(&self) -> &str;
}

/// Agent result from running an exercise
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    #[serde(rename = "exerciseName")]
    pub exercise_name: String,
    pub language: String,
    pub success: bool,
    pub exit_code: i32,
    pub output: String,
    #[serde(rename = "duration", serialize_with = "serialize_duration_seconds")]
    pub duration_ms: u64,
    #[serde(rename = "startTime")]
    pub start_time: String,
    #[serde(rename = "endTime")]
    pub end_time: String,
    #[serde(rename = "errorMessage", skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(rename = "containerId")]
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
            container_id: self.container_id.unwrap_or_default(),
        }
    }
}
