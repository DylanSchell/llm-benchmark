use chrono::DateTime;
use serde::{Deserialize, Serialize, Serializer};
use std::path::Path;
use std::str::FromStr;

/// Identifies which agent / coding assistant is being benchmarked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentKind {
    /// Copies reference solution and runs tests (baseline).
    Reference,
    /// Anthropic's Claude Code CLI.
    Claude,
    /// Pi coding agent.
    Pi,
}

impl AgentKind {
    /// All supported agent kinds.
    pub const ALL: &[AgentKind] = &[AgentKind::Reference, AgentKind::Claude, AgentKind::Pi];
}

impl FromStr for AgentKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "reference" => Ok(AgentKind::Reference),
            "claude" => Ok(AgentKind::Claude),
            "pi" => Ok(AgentKind::Pi),
            other => Err(format!(
                "Unsupported agent: '{}'. Supported: reference, claude, pi",
                other
            )),
        }
    }
}

impl std::fmt::Display for AgentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentKind::Reference => write!(f, "reference"),
            AgentKind::Claude => write!(f, "claude"),
            AgentKind::Pi => write!(f, "pi"),
        }
    }
}

/// Represents a time interval [start, end) in milliseconds.
#[derive(Debug, Clone, Copy)]
pub struct TimeInterval {
    pub start: u64,
    pub end: u64,
}

impl TimeInterval {
    pub fn duration(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }
}

/// Merges overlapping/adjacent intervals and returns total wall-clock duration.
///
/// Example: [0,100], [50,150], [200,300] → merged [0,150]+[200,300] = 200ms
/// This is the correct way to sum execution times when tasks run in parallel.
pub fn merge_intervals_and_total_duration(intervals: &[TimeInterval]) -> u64 {
    if intervals.is_empty() {
        return 0;
    }

    let mut sorted = intervals.to_vec();
    sorted.sort_by_key(|i| i.start);

    let mut total_duration: u64 = 0;
    let mut current_start = sorted[0].start;
    let mut current_end = sorted[0].end;

    for interval in &sorted[1..] {
        if interval.start <= current_end {
            // Overlapping or adjacent — extend
            current_end = current_end.max(interval.end);
        } else {
            // Disjoint — close current, start new
            total_duration += current_end.saturating_sub(current_start);
            current_start = interval.start;
            current_end = interval.end;
        }
    }

    total_duration += current_end.saturating_sub(current_start);
    total_duration
}

/// Parses an RFC3339 timestamp to Unix epoch milliseconds.
pub fn parse_rfc3339_ms(ts: &str) -> Option<u64> {
    DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|dt| dt.timestamp_millis() as u64)
}

/// Computes wall-clock statistics for a set of results by merging overlapping intervals.
pub fn compute_wall_clock_stats(results: &[AgentResult]) -> WallClockStats {
    if results.is_empty() {
        return WallClockStats::default();
    }

    let intervals: Vec<TimeInterval> = results
        .iter()
        .filter_map(|r| {
            let start_ms = parse_rfc3339_ms(&r.start_time);
            let end_ms = parse_rfc3339_ms(&r.end_time);
            match (start_ms, end_ms) {
                (Some(start), Some(end)) if end >= start => Some(TimeInterval { start, end }),
                _ => None,
            }
        })
        .collect();

    let total_duration = merge_intervals_and_total_duration(&intervals);

    let wall_start = intervals.iter().map(|i| i.start).min().unwrap_or(0);
    let wall_end = intervals.iter().map(|i| i.end).max().unwrap_or(0);
    let wall_clock_span = wall_end.saturating_sub(wall_start);

    let sum_individual: u64 = intervals.iter().map(|i| i.duration()).sum();

    WallClockStats {
        wall_clock_span,
        total_duration,
        sum_individual_durations: sum_individual,
        parallelism_gain: if wall_clock_span > 0 {
            (1.0 - (total_duration as f64 / wall_clock_span as f64)).max(0.0)
        } else {
            0.0
        },
    }
}

/// Statistics about wall-clock execution time vs individual durations.
#[derive(Debug, Clone, Default)]
pub struct WallClockStats {
    /// Earliest start to latest end across all results.
    pub wall_clock_span: u64,
    /// Sum of merged (non-overlapping) intervals — true work time.
    pub total_duration: u64,
    /// Sum of all individual durations (overcounts when parallel).
    pub sum_individual_durations: u64,
    /// Fraction of time saved by parallelism (0.0 = no overlap, ~1.0 = fully parallel).
    pub parallelism_gain: f64,
}

fn serialize_duration_seconds<S>(duration_ms: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_f64(*duration_ms as f64 / 1000.0)
}

/// Deserializes a duration value that can be an integer (milliseconds),
/// a float (seconds), or a numeric string. Always produces milliseconds.
fn deserialize_duration_ms<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i as u64)
            } else if let Some(f) = n.as_f64() {
                Ok((f * 1000.0) as u64)
            } else {
                Ok(0)
            }
        }
        serde_json::Value::String(s) => {
            Ok(s.parse::<f64>().map(|f| (f * 1000.0) as u64).unwrap_or(0))
        }
        _ => Ok(0),
    }
}

/// Deserializes a timestamp that can be a Unix epoch float, integer, or
/// RFC3339 string. Always produces an RFC3339 string.
fn deserialize_timestamp<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
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

    /// Run an exercise with a custom Docker container timeout override (in seconds).
    /// When `Some(secs)`, overrides the default container timeout. When `None`,
    /// the default from config is used.
    async fn run_exercise_with_timeout(
        &self,
        exercise: &crate::exercise::Exercise,
        host_exercise_dir: &Path,
        model: &str,
        thinking_level: Option<&str>,
        results_dir: &Path,
        timeout_override_secs: Option<u64>,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>>;

    /// Returns the agent's name (e.g., "reference", "claude", "pi").
    fn get_name(&self) -> &str;
}

/// Unified result from running an exercise through an agent.
///
/// Used at both runtime (agents produce these) and for reporting
/// (deserialized from persisted result files). Serde aliases provide
/// backward compatibility with both camelCase and snake_case formats
/// from earlier versions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    #[serde(rename = "exerciseName", alias = "exercise_name")]
    pub exercise_name: String,
    pub language: String,
    pub success: bool,
    #[serde(rename = "exitCode", alias = "exit_code", default)]
    pub exit_code: i32,
    #[serde(default)]
    pub output: String,
    #[serde(
        rename = "duration",
        alias = "durationMs",
        alias = "duration_ms",
        deserialize_with = "deserialize_duration_ms",
        serialize_with = "serialize_duration_seconds"
    )]
    pub duration_ms: u64,
    #[serde(
        rename = "startTime",
        alias = "start_time",
        deserialize_with = "deserialize_timestamp",
        default
    )]
    pub start_time: String,
    #[serde(
        rename = "endTime",
        alias = "end_time",
        deserialize_with = "deserialize_timestamp",
        default
    )]
    pub end_time: String,
    #[serde(
        rename = "errorMessage",
        alias = "error_message",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub error_message: Option<String>,
    #[serde(rename = "containerId", default)]
    pub container_id: String,

    // ── Extended fields (not present in all result files — use #[serde(default)]) ──
    #[serde(default = "default_attempts")]
    pub attempts: u64,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub trace: Option<String>,
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default, rename = "cachedInputTokens")]
    pub cached_input_tokens: u64,
    #[serde(default, rename = "uncachedInputTokens")]
    pub uncached_input_tokens: u64,
}

fn default_attempts() -> u64 {
    1
}

impl AgentResult {
    /// Parses start_time to Unix epoch milliseconds.
    pub fn start_ms(&self) -> Option<u64> {
        parse_rfc3339_ms(&self.start_time)
    }

    /// Parses end_time to Unix epoch milliseconds.
    pub fn end_ms(&self) -> Option<u64> {
        parse_rfc3339_ms(&self.end_time)
    }
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
    attempts: Option<u64>,
    model: Option<String>,
    trace: Option<String>,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cached_input_tokens: Option<u64>,
    uncached_input_tokens: Option<u64>,
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

    pub fn attempts(mut self, attempts: u64) -> Self {
        self.attempts = Some(attempts);
        self
    }

    pub fn model(mut self, model: String) -> Self {
        self.model = Some(model);
        self
    }

    pub fn trace(mut self, trace: String) -> Self {
        self.trace = Some(trace);
        self
    }

    pub fn input_tokens(mut self, tokens: u64) -> Self {
        self.input_tokens = Some(tokens);
        self
    }

    pub fn output_tokens(mut self, tokens: u64) -> Self {
        self.output_tokens = Some(tokens);
        self
    }

    pub fn cached_input_tokens(mut self, tokens: u64) -> Self {
        self.cached_input_tokens = Some(tokens);
        self
    }

    pub fn uncached_input_tokens(mut self, tokens: u64) -> Self {
        self.uncached_input_tokens = Some(tokens);
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
            attempts: self.attempts.unwrap_or(1),
            model: self.model.unwrap_or_default(),
            trace: self.trace,
            input_tokens: self.input_tokens.unwrap_or(0),
            output_tokens: self.output_tokens.unwrap_or(0),
            cached_input_tokens: self.cached_input_tokens.unwrap_or(0),
            uncached_input_tokens: self.uncached_input_tokens.unwrap_or(0),
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Existing result files store timestamps as float epoch seconds and
    /// durations as float seconds. Verify backward-compatible deserialization.
    #[test]
    fn test_deserialize_existing_format() {
        let json = r#"{
            "exerciseName": "say",
            "language": "javascript",
            "success": true,
            "exitCode": 0,
            "output": "test output",
            "duration": 19.625758,
            "startTime": 1776844386.963946,
            "endTime": 1776844406.589704
        }"#;

        let result: AgentResult = serde_json::from_str(json).expect("should deserialize existing format");
        assert_eq!(result.exercise_name, "say");
        assert_eq!(result.language, "javascript");
        assert!(result.success);
        assert_eq!(result.exit_code, 0);
        // Float seconds → ms
        assert_eq!(result.duration_ms, 19625);
        // Float epoch seconds → RFC3339 string
        assert!(!result.start_time.is_empty());
        assert!(!result.end_time.is_empty());
        assert!(result.start_time.contains("2026")); // roughly correct year
    }

    /// Older files may omit containerId, model, token counts.
    #[test]
    fn test_deserialize_minimal_fields() {
        let json = r#"{"exerciseName": "foo", "language": "java", "success": false, "exitCode": 1, "output": "", "duration": 0.5, "startTime": "", "endTime": ""}"#;
        let result: AgentResult = serde_json::from_str(json).expect("should deserialize minimal fields");
        assert_eq!(result.container_id, ""); // default
        assert_eq!(result.model, ""); // default
        assert_eq!(result.attempts, 1); // default
        assert_eq!(result.input_tokens, 0);
    }

    /// Files with null errorMessage should deserialize to None.
    #[test]
    fn test_null_error_message() {
        let json = r#"{"exerciseName": "x", "language": "go", "success": true, "exitCode": 0, "output": "", "duration": 1.0, "startTime": "", "endTime": "", "errorMessage": null}"#;
        let result: AgentResult = serde_json::from_str(json).expect("null errorMessage should work");
        assert!(result.error_message.is_none());
    }
}
