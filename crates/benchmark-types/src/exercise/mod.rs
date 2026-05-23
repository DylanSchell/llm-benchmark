use serde::{Deserialize, Deserializer, Serialize};
use std::path::PathBuf;

// Custom deserializers for backward compatibility
fn deserialize_duration_ms<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(n) => {
            // Try as integer first (milliseconds)
            if let Some(i) = n.as_i64() {
                Ok(i as u64)
            } else if let Some(f) = n.as_f64() {
                // It's a float - assume seconds, convert to milliseconds
                Ok((f * 1000.0) as u64)
            } else {
                Ok(0)
            }
        }
        serde_json::Value::String(s) => Ok(s.parse::<f64>()
            .map(|f| (f * 1000.0) as u64)
            .unwrap_or(0)),
        _ => Ok(0),
    }
}

fn deserialize_timestamp<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(n) => {
            // Unix timestamp as number (seconds or milliseconds)
            if let Some(f) = n.as_f64() {
                // If it's around 10^9, it's seconds; if 10^12, it's milliseconds
                let secs = if f > 1e12 { f / 1000.0 } else { f };
                if let Some(dt) = chrono::DateTime::from_timestamp(secs as i64, 0) {
                    Ok(dt.to_rfc3339())
                } else {
                    Ok(String::new())
                }
            } else {
                Ok(String::new())
            }
        }
        serde_json::Value::String(s) => Ok(s),
        _ => Ok(String::new()),
    }
}

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

/// Metadata for an exercise parsed from .meta/config.json
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExerciseMetadata {
    #[serde(default)]
    pub authors: Option<Vec<String>>,
    #[serde(default)]
    pub contributors: Option<Vec<String>>,
    #[serde(default)]
    pub files: Option<ExerciseFiles>,
    #[serde(default)]
    pub blurb: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default, rename = "source_url")]
    pub source_url: Option<String>,
    #[serde(default)]
    pub custom: Option<serde_json::Value>,
}

/// File categories from config.json
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExerciseFiles {
    #[serde(default)]
    pub solution: Option<Vec<String>>,
    #[serde(default)]
    pub test: Option<Vec<String>>,
    #[serde(default)]
    pub example: Option<Vec<String>>,
    #[serde(default)]
    pub editor: Option<Vec<String>>,
    #[serde(default)]
    pub invalidator: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExerciseResult {
    #[serde(alias = "exercise_name", alias = "exerciseName")]
    pub exercise_name: String,
    pub language: String,
    pub success: bool,
    #[serde(rename = "exitCode", alias = "exit_code", default)]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub output: String,
    #[serde(rename = "durationMs", alias = "duration_ms", alias = "duration", deserialize_with = "deserialize_duration_ms", default)]
    pub duration_ms: u64,
    #[serde(rename = "startTime", alias = "start_time", deserialize_with = "deserialize_timestamp", default)]
    pub start_time: String,
    #[serde(rename = "endTime", alias = "end_time", deserialize_with = "deserialize_timestamp", default)]
    pub end_time: String,
    #[serde(rename = "errorMessage", alias = "error_message", skip_serializing_if = "Option::is_none", default)]
    pub error_message: Option<String>,
    #[serde(default)]
    pub trace: Option<String>,
    
    // Token tracking fields
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default, rename = "cachedInputTokens")]
    pub cached_input_tokens: u64,
    #[serde(default, rename = "uncachedInputTokens")]
    pub uncached_input_tokens: u64,
    
    // For reporting/grouping
    #[serde(default)]
    pub model: String,
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
    input_tokens: u64,
    output_tokens: u64,
    cached_input_tokens: u64,
    uncached_input_tokens: u64,
    model: String,
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

    pub fn input_tokens(mut self, input_tokens: u64) -> Self {
        self.input_tokens = input_tokens;
        self
    }

    pub fn output_tokens(mut self, output_tokens: u64) -> Self {
        self.output_tokens = output_tokens;
        self
    }

    pub fn cached_input_tokens(mut self, cached_input_tokens: u64) -> Self {
        self.cached_input_tokens = cached_input_tokens;
        self
    }

    pub fn uncached_input_tokens(mut self, uncached_input_tokens: u64) -> Self {
        self.uncached_input_tokens = uncached_input_tokens;
        self
    }

    pub fn model(mut self, model: String) -> Self {
        self.model = model;
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
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            cached_input_tokens: self.cached_input_tokens,
            uncached_input_tokens: self.uncached_input_tokens,
            model: self.model,
        }
    }
}
