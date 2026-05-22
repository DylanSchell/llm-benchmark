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
